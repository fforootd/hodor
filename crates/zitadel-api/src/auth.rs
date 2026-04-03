use axum::{Extension, Router, response::Response, routing::get};
use serde::Serialize;

use crate::{middleware::Identity, response};

pub fn routes() -> Router {
    Router::new().route("/auth/whoami", get(whoami))
}

#[derive(Serialize)]
struct WhoAmIResponse {
    pub user_id: String,
    pub session_id: String,
    pub token_type: String,
    pub org_id: String,
}

async fn whoami(Extension(identity): Extension<Identity>) -> Response {
    response::json_ok(WhoAmIResponse {
        user_id: identity.user_id,
        session_id: identity.session_id,
        token_type: identity.token_type,
        org_id: identity.org_id,
    })
}
