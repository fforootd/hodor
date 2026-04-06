#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod adapters;
pub mod authorize;
pub mod discovery;
pub mod jwks;
pub mod oidc;
pub mod op;
pub mod rp;
pub mod rs;
pub mod stores;
pub mod token;
pub mod userinfo;

use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use zitadel_app::repo::OidcRepository;
use zitadel_authn::cookie::CookieConfig;
use zitadel_config::oidc::OidcConfig;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_storage::DefaultTransientStorage;

type DefaultProvider = op::Provider<
    adapters::ZitadelOpStore,
    adapters::ZitadelOpStore,
    stores::PersistentKeyStore,
    adapters::ZitadelOpStore,
    stores::PersistentTokenStore,
>;

#[derive(Clone)]
pub struct OidcState {
    pub provider: DefaultProvider,
    pub transient: Option<Arc<DefaultTransientStorage>>,
    pub cookie_config: Option<Arc<CookieConfig>>,
}

impl OidcState {
    pub fn new(repo: Arc<dyn OidcRepository>, issuer: String, login_path: String) -> Self {
        Self::new_with_config(repo, issuer, login_path, &OidcConfig::default())
    }

    pub fn new_with_config(
        repo: Arc<dyn OidcRepository>,
        issuer: String,
        login_path: String,
        oidc_config: &OidcConfig,
    ) -> Self {
        Self::new_for_instance(
            repo,
            issuer,
            login_path,
            DEFAULT_INSTANCE_ID.to_string(),
            oidc_config,
        )
    }

    pub fn new_for_instance(
        repo: Arc<dyn OidcRepository>,
        issuer: String,
        login_path: String,
        instance_id: String,
        oidc_config: &OidcConfig,
    ) -> Self {
        let store = adapters::ZitadelOpStore::new(repo);
        let lifetimes = op::TokenLifetimes::from(oidc_config);
        let provider = op::Provider::new(
            instance_id,
            issuer,
            login_path,
            store.clone(),
            store.clone(),
            stores::PersistentKeyStore::ephemeral(oidc_config.clone()),
            store,
            stores::PersistentTokenStore::disabled(),
        )
        .with_lifetimes(lifetimes);

        Self {
            provider,
            transient: None,
            cookie_config: None,
        }
    }

    pub fn new_runtime_with_config(
        repo: Arc<dyn OidcRepository>,
        issuer: String,
        login_path: String,
        instance_id: String,
        oidc_config: &OidcConfig,
        db: zitadel_db::Db,
        secret_box: Arc<zitadel_crypto::SecretBox>,
        transient: Arc<DefaultTransientStorage>,
        cookie_config: Arc<CookieConfig>,
    ) -> Self {
        let store = adapters::ZitadelOpStore::new(repo);
        let lifetimes = op::TokenLifetimes::from(oidc_config);
        let provider = op::Provider::new(
            instance_id,
            issuer,
            login_path,
            store.clone(),
            store.clone(),
            stores::PersistentKeyStore::new(db.clone(), secret_box, oidc_config.clone()),
            store.clone(),
            stores::PersistentTokenStore::new(db),
        )
        .with_lifetimes(lifetimes);

        Self {
            provider,
            transient: Some(transient),
            cookie_config: Some(cookie_config),
        }
    }

    pub fn with_public_origin_override(mut self, public_origin: impl AsRef<str>) -> Self {
        let public_origin = public_origin.as_ref().trim();
        if public_origin.is_empty() {
            return self;
        }

        self.provider = self
            .provider
            .clone()
            .with_issuer_override(Some(public_origin.to_string()));
        self
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
