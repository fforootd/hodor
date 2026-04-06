use crate::ApiState;
use axum::Router;

pub fn routes() -> Router<ApiState> {
    crate::generic_named_resource::routes("apps", "apps")
}
