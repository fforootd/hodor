pub mod adapters;
pub mod authorize;
pub mod discovery;
pub mod jwks;
pub mod oidc;
pub mod op;
pub mod rp;
pub mod rs;
pub mod token;
pub mod userinfo;

use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use zitadel_db::{DEFAULT_INSTANCE_ID, Db};

type DefaultProvider = op::Provider<
    adapters::ZitadelOpStore,
    adapters::ZitadelOpStore,
    adapters::RuntimeKeyStore,
    adapters::ZitadelOpStore,
>;

#[derive(Clone)]
pub struct OidcState {
    pub provider: DefaultProvider,
}

impl OidcState {
    pub fn new(db: Db, issuer: String) -> Self {
        Self::new_for_instance(db, issuer, DEFAULT_INSTANCE_ID.to_string())
    }

    pub fn new_for_instance(db: Db, issuer: String, instance_id: String) -> Self {
        let store = adapters::ZitadelOpStore::new(db);
        let keys = adapters::RuntimeKeyStore::new();
        let provider = op::Provider::new(
            instance_id,
            issuer,
            store.clone(),
            store.clone(),
            keys,
            store,
        );

        Self { provider }
    }
}

pub(crate) fn protocol_error_response(error: oidc::ProtocolError) -> Response {
    let status =
        StatusCode::from_u16(error.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(error.body)).into_response()
}

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .merge(discovery::routes(state.clone()))
        .merge(jwks::routes(state.clone()))
        .merge(authorize::routes(state.clone()))
        .merge(token::routes(state.clone()))
        .merge(userinfo::routes(state))
}
