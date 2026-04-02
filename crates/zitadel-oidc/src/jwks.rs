use axum::{Router, extract::State, response::IntoResponse, routing::get, Json};
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

async fn jwks(State(state): State<OidcState>) -> impl IntoResponse {
    let keys = state.keys.read().await;
    Json(JwksResponse {
        keys: vec![Jwk {
            kty: "RSA".into(),
            use_: "sig".into(),
            kid: keys.kid.clone(),
            alg: "RS256".into(),
            n: keys.n.clone(),
            e: keys.e.clone(),
        }],
    })
}
