use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use uuid::Uuid;
use zitadel_db::{
    delete_provider, first_org_id,
    provider::{self, ProviderPayload, ProviderRecord},
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/providers", get(list).post(create))
        .route(
            "/providers/{id}",
            get(get_one).patch(update).delete(
                |state: State<ApiState>, path: Path<String>| async move {
                    delete_one(state, path).await
                },
            ),
        )
}

async fn create(State(s): State<ApiState>, Json(req): Json<ProviderPayload>) -> Response {
    let instance_id = zitadel_db::current_instance_id();
    let id = Uuid::new_v4().to_string();
    let org_id = match first_org_id(&s.db, instance_id.as_ref()).await {
        Ok(Some(org_id)) => org_id,
        Ok(None) => return response::internal_error("no org found"),
        Err(error) => return response::internal_error(format!("{error}")),
    };

    match provider::insert_provider_for(&s.db, instance_id.as_ref(), &id, &org_id, &req).await {
        Ok(()) => match provider::get_provider_for(&s.db, instance_id.as_ref(), &id).await {
            Ok(Some(record)) => response::json_created(record),
            Ok(None) => response::internal_error("provider created but could not be reloaded"),
            Err(error) => response::internal_error(format!("{error}")),
        },
        Err(error) => response::bad_request(format!("{error}")),
    }
}

async fn list(State(s): State<ApiState>) -> Response {
    let instance_id = zitadel_db::current_instance_id();
    match provider::list_providers_for(&s.db, instance_id.as_ref()).await {
        Ok(items) => response::json_ok(serde_json::json!({
            "providers": items,
            "items": items,
            "total": items.len(),
        })),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let instance_id = zitadel_db::current_instance_id();
    match provider::get_provider_for(&s.db, instance_id.as_ref(), &id).await {
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
    let instance_id = zitadel_db::current_instance_id();
    match provider::update_provider_for(&s.db, instance_id.as_ref(), &id, &req).await {
        Ok(true) => match provider::get_provider_for(&s.db, instance_id.as_ref(), &id).await {
            Ok(Some(record)) => response::json_ok(record),
            Ok(None) => response::not_found("provider not found"),
            Err(error) => response::internal_error(format!("{error}")),
        },
        Ok(false) => response::not_found("provider not found"),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let instance_id = zitadel_db::current_instance_id();
    match delete_provider(&s.db, instance_id.as_ref(), &id).await {
        Ok(false) => response::not_found("provider not found"),
        Ok(true) => response::no_content(),
        Err(error) => response::internal_error(format!("{error}")),
    }
}

#[allow(dead_code)]
fn _assert_provider_record_send_sync(_: &ProviderRecord) {}
