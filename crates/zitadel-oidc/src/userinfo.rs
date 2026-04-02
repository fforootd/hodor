use axum::{Router, extract::State, http::{StatusCode, header}, response::{IntoResponse, Response}, routing::get, Json};
use jsonwebtoken::{Validation, Algorithm, DecodingKey};
use serde::{Deserialize, Serialize};
use crate::OidcState;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/userinfo", get(userinfo))
        .with_state(state)
}

#[derive(Serialize)]
struct UserinfoResponse {
    sub: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    email: String,
}

#[derive(Deserialize)]
struct TokenClaims { sub: String }

async fn userinfo(State(state): State<OidcState>, req: axum::extract::Request) -> Response {
    // Extract Bearer token.
    let auth = match req.headers().get(header::AUTHORIZATION) {
        Some(v) => v.to_str().unwrap_or("").to_string(),
        None => return (StatusCode::UNAUTHORIZED, "Bearer token required").into_response(),
    };
    let token = match auth.strip_prefix("Bearer ") {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Bearer token required").into_response(),
    };

    // Decode token to get subject.
    let keys = state.keys.read().await;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&state.issuer]);
    validation.validate_aud = false; // relaxed for POC
    validation.validate_exp = false; // relaxed for POC

    let claims: TokenClaims = match jsonwebtoken::decode(token, &keys.decoding, &validation) {
        Ok(data) => data.claims,
        Err(_) => return (StatusCode::UNAUTHORIZED, "invalid token").into_response(),
    };

    // Load user.
    let scoped = state.db.scoped_default();
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT identifier, display_name FROM users WHERE instance_id = ? AND id = ?",
    )
    .bind(scoped.instance_id())
    .bind(&claims.sub)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);

    let (email, name) = row.unwrap_or_default();

    Json(UserinfoResponse { sub: claims.sub, name, email }).into_response()
}
