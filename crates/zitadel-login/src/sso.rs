use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use zitadel_db::{
    DEFAULT_INSTANCE_ID,
    provider::{self, ProviderLinkingMode, ProviderMatchBy, ProviderPayload, ProviderRecord},
};
use zitadel_oidc::rp::{RpAuthState, RpCallbackRequest, RpProviderSpec, RpStartRequest, StateStore};
use zitadel_storage::ProviderAuthState;

use crate::LoginState;

pub fn routes() -> Router<LoginState> {
    Router::new()
        .route("/v1/auth/sso/{provider_id}/start", get(sso_start))
        .route("/v1/auth/sso/callback", get(sso_callback))
}

#[derive(Clone)]
struct TransientRpStateStore {
    transient: Arc<zitadel_storage::DefaultTransientStorage>,
}

impl StateStore for TransientRpStateStore {
    async fn put_state(&self, instance_id: &str, state: &RpAuthState) -> anyhow::Result<()> {
        self.transient
            .create_provider_auth_state(
                instance_id,
                &ProviderAuthState {
                    provider_id: state.provider_id.clone(),
                    state: state.state.clone(),
                    nonce: state.nonce.clone(),
                    pkce_verifier: state.pkce_verifier.clone(),
                    flow_id: state.flow_id.clone(),
                    redirect_uri: state.redirect_uri.clone(),
                    expected_issuer: state.expected_issuer.clone(),
                    callback_uri: state.callback_uri.clone(),
                },
            )
            .await
    }

    async fn take_state(&self, instance_id: &str, state: &str) -> anyhow::Result<Option<RpAuthState>> {
        Ok(self
            .transient
            .consume_provider_auth_state(instance_id, state)
            .await?
            .map(|stored| RpAuthState {
                provider_id: stored.provider_id,
                state: stored.state,
                nonce: stored.nonce,
                pkce_verifier: stored.pkce_verifier,
                flow_id: stored.flow_id,
                redirect_uri: stored.redirect_uri,
                expected_issuer: stored.expected_issuer,
                callback_uri: stored.callback_uri,
            }))
    }
}

#[derive(Deserialize)]
struct SsoStartParams {
    #[serde(default)]
    flow_id: String,
    #[serde(default)]
    redirect_uri: String,
}

async fn sso_start(
    State(state): State<LoginState>,
    Path(provider_id): Path<String>,
    Query(params): Query<SsoStartParams>,
) -> Response {
    let scoped = state.db.scoped_default();
    let provider = match provider::get_provider(&scoped, &provider_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "provider not found"})),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load provider: {error}")})),
            )
                .into_response();
        }
    };

    if !provider.payload.enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "provider disabled"})),
        )
            .into_response();
    }
    if provider.payload.protocol != "oidc" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "provider protocol is not supported by this endpoint"})),
        )
            .into_response();
    }

    let spec = match provider_to_rp_spec(&provider) {
        Ok(spec) => spec,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("{error}")})),
            )
                .into_response();
        }
    };

    let callback_uri = format!(
        "{}/v1/auth/sso/callback",
        state.public_origin.trim_end_matches('/')
    );
    let store = TransientRpStateStore {
        transient: state.transient.clone(),
    };
    let result = match state
        .rp
        .start_with_store(
            DEFAULT_INSTANCE_ID,
            &RpStartRequest {
                provider: spec,
                flow_id: params.flow_id,
                redirect_uri: params.redirect_uri,
                callback_uri,
            },
            &store,
        )
        .await
    {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(provider_id = %provider.id, %error, "failed to start RP login");
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": format!("OIDC start failed: {error}")})),
            )
                .into_response();
        }
    };

    Redirect::temporary(&result.authorization_url).into_response()
}

#[derive(Deserialize)]
struct SsoCallbackParams {
    #[serde(default)]
    code: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

async fn sso_callback(
    State(state): State<LoginState>,
    Query(params): Query<SsoCallbackParams>,
) -> Response {
    if !params.error.is_empty() {
        tracing::warn!(
            error = %params.error,
            description = %params.error_description,
            "OIDC RP upstream returned error"
        );
        return redirect_login_error("sso_failed", &params.error_description);
    }
    if params.state.is_empty() || params.code.is_empty() {
        return redirect_login_error("sso_failed", "missing state or code");
    }

    let store = TransientRpStateStore {
        transient: state.transient.clone(),
    };
    let stored_state = match store.take_state(DEFAULT_INSTANCE_ID, &params.state).await {
        Ok(Some(state)) => state,
        Ok(None) => return redirect_login_error("sso_failed", "invalid or expired state"),
        Err(error) => {
            tracing::error!(%error, "failed to load RP auth state");
            return redirect_login_error("sso_failed", "state lookup failed");
        }
    };

    let scoped = state.db.scoped_default();
    let provider = match provider::get_provider(&scoped, &stored_state.provider_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => return redirect_login_error("sso_failed", "provider not found"),
        Err(error) => {
            tracing::error!(provider_id = %stored_state.provider_id, %error, "failed to reload provider");
            return redirect_login_error("sso_failed", "provider lookup failed");
        }
    };
    let spec = match provider_to_rp_spec(&provider) {
        Ok(spec) => spec,
        Err(error) => return redirect_login_error("sso_failed", &error.to_string()),
    };

    let identity = match state
        .rp
        .finish(&RpCallbackRequest {
            provider: spec,
            stored_state: stored_state.clone(),
            returned_state: params.state,
            code: params.code,
        })
        .await
    {
        Ok(identity) => identity,
        Err(error) => {
            let description = error.to_string();
            let error_code = if description.to_ascii_lowercase().contains("nonce") {
                "sso_nonce"
            } else {
                "sso_token"
            };
            tracing::error!(provider_id = %provider.id, error_code, %description, "OIDC RP callback failed");
            return redirect_login_error(error_code, &description);
        }
    };

    match complete_federated_login(&state, &provider, &stored_state, &identity).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(provider_id = %provider.id, %error, "federated login failed");
            redirect_login_error("sso_link_failed", &error.to_string())
        }
    }
}

fn provider_to_rp_spec(provider: &ProviderRecord) -> anyhow::Result<RpProviderSpec> {
    let connection = &provider.payload.connection;
    if connection.issuer.is_empty() || connection.client_id.is_empty() {
        anyhow::bail!("provider missing issuer or client_id");
    }

    Ok(RpProviderSpec {
        provider_id: provider.id.clone(),
        issuer: connection.issuer.clone(),
        client_id: connection.client_id.clone(),
        client_secret: connection.client_secret.clone(),
        scopes: connection.scopes.clone(),
        token_endpoint_auth_method: connection.token_endpoint_auth_method.clone(),
    })
}

async fn complete_federated_login(
    state: &LoginState,
    provider: &ProviderRecord,
    stored_state: &RpAuthState,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
) -> anyhow::Result<Response> {
    let scoped = state.db.scoped_default();
    let schema = load_target_schema(&scoped, &provider.payload).await?;
    let defaults = schema
        .as_ref()
        .map(zitadel_schema::claim_defaults)
        .unwrap_or_default();
    let profile = zitadel_expr::map_claims(&defaults, &provider.payload.mapping.claims, &identity.claims);
    let user_id = find_or_create_identity(&scoped, provider, identity, &profile).await?;

    let org_id = sqlx::query_as::<_, (String,)>(
        "SELECT org_id FROM users WHERE instance_id = $1 AND id = $2",
    )
    .bind(scoped.instance_id())
    .bind(&user_id)
    .fetch_optional(scoped.pool())
    .await?
    .map(|row| row.0)
    .unwrap_or_default();

    let created = state
        .transient
        .create_session(DEFAULT_INSTANCE_ID, &user_id, &org_id, "", "")
        .await?;

    let metadata = serde_json::json!({
        "auth_method": "sso",
        "provider_id": provider.id,
        "provider_kind": provider.payload.kind,
        "login_flow_id": stored_state.flow_id,
        "auth_context": {
            "issuer": identity.issuer,
            "subject": identity.subject
        }
    });
    let sql = format!(
        "UPDATE sessions SET metadata = {} WHERE instance_id = $1 AND id = $2",
        scoped.json_bind(3),
    );
    sqlx::query(&sql)
        .bind(scoped.instance_id())
        .bind(&created.session_id)
        .bind(serde_json::to_string(&metadata)?)
        .execute(scoped.pool())
        .await?;

    let signed = zitadel_auth::cookie::sign(&created.token, &state.cookie_config.secrets[0]);
    let cookie_name = state.cookie_config.cookie_name();
    let secure_flag = if state.cookie_config.secure { "; Secure" } else { "" };
    let cookie_value = format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        zitadel_auth::cookie::MAX_AGE,
    );

    let redirect_url = "/login?sso=complete";
    let mut response = Redirect::temporary(redirect_url).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie_value.parse().expect("valid cookie header"),
    );
    Ok(response)
}

async fn load_target_schema(
    scoped: &zitadel_db::scoped::ScopedDb,
    provider: &ProviderPayload,
) -> anyhow::Result<Option<serde_json::Value>> {
    let schema_column = scoped.as_text("schema");
    let schema_row = if !provider.target.schema_id.is_empty() {
        let sql = format!("SELECT {schema_column} FROM schemas WHERE id = $1 LIMIT 1");
        sqlx::query_as::<_, (String,)>(&sql)
            .bind(&provider.target.schema_id)
            .fetch_optional(scoped.pool())
            .await?
    } else if !provider.target.schema_type.is_empty() {
        let sql = format!(
            "SELECT {schema_column} FROM schemas WHERE type = $1 AND is_default = 1 ORDER BY version DESC LIMIT 1"
        );
        sqlx::query_as::<_, (String,)>(&sql)
            .bind(&provider.target.schema_type)
            .fetch_optional(scoped.pool())
            .await?
    } else {
        None
    };

    if let Some((schema_json,)) = schema_row {
        return Ok(serde_json::from_str(&schema_json).ok());
    }

    Ok(zitadel_schema::bundled_schema(
        if provider.target.schema_type.is_empty() {
            "human_user"
        } else {
            &provider.target.schema_type
        },
    ))
}

async fn find_or_create_identity(
    scoped: &zitadel_db::scoped::ScopedDb,
    provider: &ProviderRecord,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
    profile: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<String> {
    if let Some((user_id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT user_id FROM linked_identities WHERE instance_id = $1 AND provider_id = $2 AND external_sub = $3",
    )
    .bind(scoped.instance_id())
    .bind(&provider.id)
    .bind(&identity.subject)
    .fetch_optional(scoped.pool())
    .await?
    {
        let raw_claims = serde_json::to_string(&identity.claims)?;
        let update_sql = format!(
            "UPDATE linked_identities SET last_used_at = CURRENT_TIMESTAMP, external_email = $1, raw_claims = {} \
             WHERE instance_id = $2 AND provider_id = $3 AND external_sub = $4",
            scoped.json_bind(5),
        );
        sqlx::query(&update_sql)
            .bind(&identity.email)
            .bind(scoped.instance_id())
            .bind(&provider.id)
            .bind(&identity.subject)
            .bind(raw_claims)
            .execute(scoped.pool())
            .await?;
        return Ok(user_id);
    }

    if let Some(existing_user_id) = match_existing_user(scoped, provider, identity, profile).await? {
        create_linked_identity(scoped, &existing_user_id, provider, identity).await?;
        return Ok(existing_user_id);
    }

    if provider.payload.linking.mode == ProviderLinkingMode::LinkOnly {
        anyhow::bail!("no linked account found and provider is link_only");
    }

    let schema_id = resolve_target_schema_id(scoped, &provider.payload).await?;
    let identifier = profile_string(profile, "email")
        .or_else(|| profile_string(profile, "username"))
        .unwrap_or_else(|| {
            if identity.email.is_empty() {
                identity.subject.clone()
            } else {
                identity.email.clone()
            }
        });
    let display_name = profile_string(profile, "display_name")
        .unwrap_or_else(|| identifier.clone());
    let org_id = get_default_org(scoped).await?;
    let user_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id) \
         VALUES ($1, $2, $3, $4, $5, 'human', 'active', $6)",
    )
    .bind(&user_id)
    .bind(scoped.instance_id())
    .bind(&org_id)
    .bind(&identifier)
    .bind(&display_name)
    .bind(&schema_id)
    .execute(scoped.pool())
    .await?;

    create_linked_identity(scoped, &user_id, provider, identity).await?;
    Ok(user_id)
}

async fn match_existing_user(
    scoped: &zitadel_db::scoped::ScopedDb,
    provider: &ProviderRecord,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
    profile: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<Option<String>> {
    let candidate_identifier = match provider.payload.linking.match_by {
        ProviderMatchBy::VerifiedEmail => {
            if identity.email_verified && !identity.email.is_empty() {
                Some(identity.email.clone())
            } else {
                None
            }
        }
        ProviderMatchBy::Identifier => profile_string(profile, "email")
            .or_else(|| profile_string(profile, "username"))
            .or_else(|| {
                if identity.email.is_empty() {
                    None
                } else {
                    Some(identity.email.clone())
                }
            }),
        ProviderMatchBy::None => None,
    };

    let Some(identifier) = candidate_identifier else {
        return Ok(None);
    };

    Ok(sqlx::query_as::<_, (String,)>(
        "SELECT id FROM users WHERE instance_id = $1 AND identifier = $2 AND state = 'active'",
    )
    .bind(scoped.instance_id())
    .bind(identifier)
    .fetch_optional(scoped.pool())
    .await?
    .map(|row| row.0))
}

async fn create_linked_identity(
    scoped: &zitadel_db::scoped::ScopedDb,
    user_id: &str,
    provider: &ProviderRecord,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
) -> anyhow::Result<()> {
    let raw_claims = serde_json::to_string(&identity.claims)?;
    let sql = format!(
        "INSERT INTO linked_identities (id, instance_id, user_id, provider_id, external_sub, external_email, raw_claims) \
         VALUES ($1, $2, $3, $4, $5, $6, {})",
        scoped.json_bind(7),
    );
    sqlx::query(&sql)
        .bind(Uuid::new_v4().to_string())
        .bind(scoped.instance_id())
        .bind(user_id)
        .bind(&provider.id)
        .bind(&identity.subject)
        .bind(&identity.email)
        .bind(raw_claims)
        .execute(scoped.pool())
        .await?;
    Ok(())
}

async fn resolve_target_schema_id(
    scoped: &zitadel_db::scoped::ScopedDb,
    provider: &ProviderPayload,
) -> anyhow::Result<String> {
    if !provider.target.schema_id.is_empty() {
        return Ok(provider.target.schema_id.clone());
    }
    if provider.target.schema_type.is_empty() {
        return Ok(String::new());
    }

    let row = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM schemas WHERE type = $1 AND is_default = 1 ORDER BY version DESC LIMIT 1",
    )
    .bind(&provider.target.schema_type)
    .fetch_optional(scoped.pool())
    .await?;
    Ok(row.map(|result| result.0).unwrap_or_default())
}

async fn get_default_org(scoped: &zitadel_db::scoped::ScopedDb) -> anyhow::Result<String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM orgs WHERE instance_id = $1 LIMIT 1")
            .bind(scoped.instance_id())
            .fetch_optional(scoped.pool())
            .await?;
    row.map(|r| r.0)
        .ok_or_else(|| anyhow::anyhow!("no org found"))
}

fn profile_string(
    profile: &HashMap<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    profile
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn redirect_login_error(code: &str, description: &str) -> Response {
    let url = format!(
        "/login?error={}&error_description={}",
        urlencoding_encode(code),
        urlencoding_encode(description),
    );
    Redirect::temporary(&url).into_response()
}

fn urlencoding_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DefaultRpService;
    use zitadel_auth::{cookie::CookieConfig, password::Passwords};
    use zitadel_storage::{DefaultStatefulStorage, DefaultTransientStorage, NoopEdgeSink, SqlEdgeReadDb, SqlStateDb, SqlTransientCompatKv};

    async fn test_state() -> LoginState {
        let db = zitadel_db::Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind("org-1")
            .bind(scoped.instance_id())
            .bind("Default")
            .execute(scoped.pool())
            .await
            .unwrap();

        LoginState {
            db: db.clone(),
            stateful: Arc::new(DefaultStatefulStorage::new(
                SqlStateDb::new(db.clone()),
                SqlEdgeReadDb::new(db.clone()),
            )),
            transient: Arc::new(DefaultTransientStorage::new(
                SqlTransientCompatKv::new(db.clone()),
                NoopEdgeSink,
            )),
            passwords: Arc::new(Passwords::new_dev()),
            cookie_config: Arc::new(CookieConfig::new(
                vec!["test-secret".into()],
                "localhost",
                false,
            )),
            public_origin: Arc::new("http://localhost:8080".into()),
            rp: Arc::new(DefaultRpService::new(
                zitadel_oidc::rp::ReqwestHttpClient::new(),
                zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
            )),
        }
    }

    fn provider_payload(mode: ProviderLinkingMode) -> ProviderPayload {
        ProviderPayload {
            display_name: "Mock OIDC".into(),
            kind: "custom".into(),
            protocol: "oidc".into(),
            connection: zitadel_db::provider::ProviderConnection {
                issuer: "https://issuer.example".into(),
                client_id: "client-1".into(),
                client_secret: "secret-1".into(),
                scopes: vec!["openid".into(), "profile".into(), "email".into()],
                ..zitadel_db::provider::ProviderConnection::default()
            },
            mapping: zitadel_db::provider::ProviderMapping {
                claims: HashMap::from([
                    ("email".into(), "claims.email".into()),
                    ("display_name".into(), "claims.name".into()),
                ]),
            },
            target: zitadel_db::provider::ProviderTarget {
                schema_type: "human_user".into(),
                schema_id: String::new(),
            },
            linking: zitadel_db::provider::ProviderLinking {
                mode,
                match_by: ProviderMatchBy::VerifiedEmail,
            },
            ..ProviderPayload::default()
        }
    }

    #[tokio::test]
    async fn link_only_rejects_unknown_user() {
        let state = test_state().await;
        let scoped = state.db.scoped_default();
        provider::insert_provider(&scoped, "provider-1", "org-1", &provider_payload(ProviderLinkingMode::LinkOnly))
            .await
            .unwrap();
        let provider = provider::get_provider(&scoped, "provider-1").await.unwrap().unwrap();
        let identity = zitadel_oidc::rp::VerifiedExternalIdentity {
            issuer: "https://issuer.example".into(),
            subject: "ext-1".into(),
            email: "nobody@example.com".into(),
            email_verified: true,
            claims: serde_json::json!({
                "sub": "ext-1",
                "email": "nobody@example.com",
                "email_verified": true,
                "name": "Nobody"
            }),
            id_token: None,
            access_token: "token".into(),
        };
        let stored_state = RpAuthState {
            provider_id: provider.id.clone(),
            state: "state-1".into(),
            nonce: "nonce-1".into(),
            pkce_verifier: "verifier-1".into(),
            flow_id: "flow-1".into(),
            redirect_uri: "/console".into(),
            expected_issuer: "https://issuer.example".into(),
            callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
        };

        let error = complete_federated_login(&state, &provider, &stored_state, &identity)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("link_only"));
    }
}
