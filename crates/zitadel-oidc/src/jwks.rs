use axum::{Router, extract::State, http::StatusCode, response::{IntoResponse, Response}, routing::get, Json};
use serde::Serialize;
use crate::OidcState;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/keys", get(jwks))
        .with_state(state)
}

#[derive(Serialize)]
struct JwksResponse { keys: Vec<Jwk> }

#[derive(Serialize)]
struct Jwk {
    kty: String,
    #[serde(rename = "use")]
    use_: String,
    kid: String,
    alg: String,
    n: String,
    e: String,
}

async fn jwks(State(state): State<OidcState>) -> Response {
    let guard = match state.signing_keys().await {
        Ok(g) => g,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "signing keys not ready").into_response(),
    };
    let keys = guard.as_ref().unwrap();
    Json(JwksResponse {
        keys: vec![Jwk {
            kty: "RSA".into(),
            use_: "sig".into(),
            kid: keys.kid.clone(),
            alg: "RS256".into(),
            n: keys.n.clone(),
            e: keys.e.clone(),
        }],
    }).into_response()
}
