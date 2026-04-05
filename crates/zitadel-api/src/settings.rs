use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_app::settings::UpdateSettingsCommand;

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

async fn get_settings(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(type_): Path<String>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.get_settings.execute(&ctx, &type_, None, None).await {
        Ok(record) => response::json_ok(SettingsResponse {
            type_: record.settings_type,
            scope: record.scope,
            data: record.data,
        }),
        Err(e) => response::app_error(e),
    }
}

async fn put_settings(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(type_): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateSettingsCommand {
        settings_type: type_.clone(),
        scope: "instance".to_string(),
        data: data.clone(),
    };
    match s.app.update_settings.execute(&ctx, cmd).await {
        Ok(()) => response::json_ok(SettingsResponse {
            type_,
            scope: "instance".into(),
            data,
        }),
        Err(e) => response::app_error(e),
    }
}

async fn delete_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    // No delete_settings use case — keep direct DB call.
    // TODO(CLAUDE-4): Add DeleteSettings use case.
    match zitadel_db::delete_settings_record(
        &s.db,
        zitadel_db::current_instance_id().as_ref(),
        &type_,
    )
    .await
    {
        Ok(_) => response::no_content(),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
