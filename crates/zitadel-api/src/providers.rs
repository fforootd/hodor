use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use uuid::Uuid;
use zitadel_db::provider::{self, ProviderPayload, ProviderRecord};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/providers", get(list).post(create))
        .route(
            "/providers/{id}",
            get(get_one).patch(update).delete(delete_one),
        )
}

async fn create(State(s): State<ApiState>, Json(req): Json<ProviderPayload>) -> Response {
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let org_id = match default_org_id(&scoped).await {
        Ok(org_id) => org_id,
        Err(error) => return response::internal_error(format!("{error}")),
    };

    match provider::insert_provider(&scoped, &id, &org_id, &req).await {
        Ok(()) => match provider::get_provider(&scoped, &id).await {
            Ok(Some(record)) => response::json_created(record),
            Ok(None) => response::internal_error("provider created but could not be reloaded"),
            Err(error) => response::internal_error(format!("{error}")),
        },
        Err(error) => response::bad_request(format!("{error}")),
    }
}

async fn list(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    match provider::list_providers(&scoped).await {
        Ok(items) => response::json_ok(serde_json::json!({
            "providers": items,
            "items": items,
            "total": items.len(),
        })),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match provider::get_provider(&scoped, &id).await {
        Ok(Some(record)) => response::json_ok(record),
        Ok(None) => response::not_found("provider not found"),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn update(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<ProviderPayload>,
) -> Response {
    let scoped = s.db.scoped_default();
    match provider::update_provider(&scoped, &id, &req).await {
        Ok(true) => match provider::get_provider(&scoped, &id).await {
            Ok(Some(record)) => response::json_ok(record),
            Ok(None) => response::not_found("provider not found"),
            Err(error) => response::internal_error(format!("{error}")),
        },
        Ok(false) => response::not_found("provider not found"),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("DELETE FROM providers WHERE instance_id = $1 AND id = $2")
        .bind(scoped.instance_id())
        .bind(&id)
        .execute(scoped.pool())
        .await
    {
        Ok(result) if result.rows_affected() == 0 => response::not_found("provider not found"),
        Ok(_) => response::no_content(),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn default_org_id(scoped: &zitadel_db::scoped::ScopedDb) -> anyhow::Result<String> {
    let org_id = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM orgs WHERE instance_id = $1 ORDER BY created_at ASC LIMIT 1",
    )
    .bind(scoped.instance_id())
    .fetch_optional(scoped.pool())
    .await?
    .map(|row| row.0)
    .unwrap_or_default();

    Ok(org_id)
}

#[allow(dead_code)]
fn _assert_provider_record_send_sync(_: &ProviderRecord) {}
