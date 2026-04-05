use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_db::{
    current_instance_id, delete_settings_record, get_settings_record, put_instance_settings,
};

pub fn routes() -> Router<ApiState> {
    Router::new().route(
        "/settings/{type_}",
        get(get_settings).put(put_settings).delete(delete_settings),
    )
}

#[derive(Serialize)]
struct SettingsResponse {
    #[serde(rename = "type")]
    type_: String,
    scope: String,
    data: serde_json::Value,
}

async fn get_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    match get_settings_record(&s.db, current_instance_id().as_ref(), &type_).await {
        Ok(Some(r)) => response::json_ok(SettingsResponse {
            type_: r.type_,
            scope: r.scope,
            data: serde_json::from_str(&r.data_json).unwrap_or_default(),
        }),
        Ok(None) => response::not_found(format!("settings '{type_}' not found")),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn put_settings(
    State(s): State<ApiState>,
    Path(type_): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".into());
    let id = uuid::Uuid::new_v4().to_string();
    match put_instance_settings(&s.db, current_instance_id().as_ref(), &id, &type_, &data_str)
        .await
    {
        Ok(_) => response::json_ok(SettingsResponse {
            type_,
            scope: "instance".into(),
            data,
        }),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    match delete_settings_record(&s.db, current_instance_id().as_ref(), &type_).await {
        Ok(_) => response::no_content(),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
