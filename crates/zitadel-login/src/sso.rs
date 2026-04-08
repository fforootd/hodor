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
use zitadel_app::credentials::LinkIdentityCommand;
use zitadel_app::repo::{
    ProviderDefinitionRecord, ProviderLinkingMode, ProviderMatchBy, ProviderPayload,
};
use zitadel_app::users::CreateUserCommand;
use zitadel_db::current_instance_id;

/// Load a provider via repos as an app-owned typed definition.
async fn load_provider_via_repos(
    repos: &zitadel_app::repo::Repositories,
    instance_id: &str,
    provider_id: &str,
) -> anyhow::Result<Option<ProviderDefinitionRecord>> {
    repos
        .providers
        .get_definition(instance_id, provider_id)
        .await
}
use zitadel_oidc::rp::{
    RpAuthState, RpCallbackRequest, RpProviderSpec, RpStartRequest, StateStore,
};
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

    async fn take_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<RpAuthState>> {
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
    let instance_id = current_instance_id();
    let provider = match load_provider_via_repos(&state.app.repos, &instance_id, &provider_id).await
    {
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
            Json(
                serde_json::json!({"error": "provider protocol is not supported by this endpoint"}),
            ),
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

    let callback_uri = sso_callback_uri(&state);
    let store = TransientRpStateStore {
        transient: state.transient.clone(),
    };
    let result = match state
        .rp
        .start_with_store(
            &instance_id,
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

fn sso_callback_uri(state: &LoginState) -> String {
    format!(
        "{}/v1/auth/sso/callback",
        state.effective_public_origin().trim_end_matches('/')
    )
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
    let instance_id = current_instance_id();
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
    let stored_state = match store.take_state(&instance_id, &params.state).await {
        Ok(Some(state)) => state,
        Ok(None) => return redirect_login_error("sso_failed", "invalid or expired state"),
        Err(error) => {
            tracing::error!(%error, "failed to load RP auth state");
            return redirect_login_error("sso_failed", "state lookup failed");
        }
    };

    let provider = match load_provider_via_repos(
        &state.app.repos,
        &instance_id,
        &stored_state.provider_id,
    )
    .await
    {
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

fn provider_to_rp_spec(provider: &ProviderDefinitionRecord) -> anyhow::Result<RpProviderSpec> {
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

/// Build an [`ActorContext`] for the login flow.
///
/// During federated login, no user session exists yet, so the identity fields
/// are empty. The instance is resolved from the thread-local context set by
/// the instance middleware.
fn login_actor_context() -> zitadel_app::ActorContext {
    let instance_id = current_instance_id().into_owned();
    zitadel_app::ActorContext {
        auth: zitadel_app::context::AuthContext {
            identity: zitadel_app::context::Identity {
                user_id: String::new(),
                principal_ref: String::new(),
                session_id: String::new(),
                token_type: "login_flow".to_string(),
                org_id: String::new(),
                issuer_instance_id: None,
                support_grant: None,
            },
            capabilities: vec![],
        },
        instance: zitadel_app::context::InstanceContext {
            instance_id,
            placement_mode: String::new(),
            region_key: None,
            feature_overrides: Default::default(),
            host: String::new(),
        },
    }
}

async fn complete_federated_login(
    state: &LoginState,
    provider: &ProviderDefinitionRecord,
    stored_state: &RpAuthState,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
) -> anyhow::Result<Response> {
    let instance_id = current_instance_id();
    let repos = &state.app.repos;
    let schema = load_target_schema(repos, instance_id.as_ref(), &provider.payload).await?;
    let defaults = schema
        .as_ref()
        .map(zitadel_schema::claim_defaults)
        .unwrap_or_default();
    let profile = zitadel_expr::map_claims(
        &defaults,
        &provider.payload.mapping.claims,
        &identity.claims,
    );
    let user_id = find_or_create_identity(
        &state.app,
        instance_id.as_ref(),
        provider,
        identity,
        &profile,
    )
    .await?;

    // Issue session via the IssueSession use case (ADR-032).
    let actor = login_actor_context();
    let created = state
        .app
        .issue_session
        .execute(
            &actor,
            zitadel_app::auth::IssueSessionCommand {
                user_id: user_id.clone(),
                auth_method: "sso".to_string(),
                user_agent: String::new(),
                ip_address: String::new(),
                fingerprint: String::new(),
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("issue_session use case failed: {e}"))?;

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
    let metadata_json = serde_json::to_string(&metadata)?;
    repos
        .sessions
        .update_metadata(instance_id.as_ref(), &created.session_id, &metadata_json)
        .await?;

    let signed = zitadel_authn::cookie::sign(&created.token, &state.cookie_config.secrets[0]);
    let cookie_name = state.cookie_config.cookie_name();
    let secure_flag = if state.cookie_config.secure {
        "; Secure"
    } else {
        ""
    };
    let cookie_value = format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        state.cookie_config.max_age,
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
    repos: &zitadel_app::repo::Repositories,
    instance_id: &str,
    provider: &ProviderPayload,
) -> anyhow::Result<Option<serde_json::Value>> {
    let schema_json = if !provider.target.schema_id.is_empty() {
        repos
            .schemas
            .get(instance_id, &provider.target.schema_id)
            .await?
            .map(|record| record.schema_json)
    } else if !provider.target.schema_type.is_empty() {
        repos
            .schemas
            .get_by_type(instance_id, &provider.target.schema_type)
            .await?
            .map(|record| record.schema_json)
    } else {
        None
    };

    if let Some(schema_json) = schema_json {
        return Ok(Some(schema_json));
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
    app: &zitadel_app::ApplicationServices,
    instance_id: &str,
    provider: &ProviderDefinitionRecord,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
    profile: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<String> {
    let repos = &app.repos;

    // Check if this external identity is already linked to a user.
    if let Some(linked) = repos
        .credentials
        .find_by_external_sub(instance_id, &provider.id, &identity.subject)
        .await?
    {
        let raw_claims = serde_json::to_string(&identity.claims)?;
        repos
            .credentials
            .touch_linked_identity(
                instance_id,
                &provider.id,
                &identity.subject,
                &identity.email,
                &raw_claims,
            )
            .await?;
        return Ok(linked.user_id);
    }

    // Try to match an existing user by identifier strategy.
    if let Some(existing_user_id) =
        match_existing_user(repos, instance_id, provider, identity, profile).await?
    {
        create_linked_identity(app, instance_id, &existing_user_id, provider, identity).await?;
        return Ok(existing_user_id);
    }

    if provider.payload.linking.mode == ProviderLinkingMode::LinkOnly {
        anyhow::bail!("no linked account found and provider is link_only");
    }

    // Create a new user and link the identity.
    let schema_id = resolve_target_schema_id(repos, instance_id, &provider.payload).await?;
    let identifier = profile_string(profile, "email")
        .or_else(|| profile_string(profile, "username"))
        .unwrap_or_else(|| {
            if identity.email.is_empty() {
                identity.subject.clone()
            } else {
                identity.email.clone()
            }
        });
    let display_name =
        profile_string(profile, "display_name").unwrap_or_else(|| identifier.clone());

    let actor = login_actor_context();
    let created_user = app
        .create_user
        .execute(
            &actor,
            CreateUserCommand {
                identifier: identifier.clone(),
                display_name,
                user_type: "human_user".to_string(),
                schema_id,
                org_id: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("create_user use case failed: {e}"))?;

    create_linked_identity(app, instance_id, &created_user.id, provider, identity).await?;
    Ok(created_user.id)
}

async fn match_existing_user(
    repos: &zitadel_app::repo::Repositories,
    instance_id: &str,
    provider: &ProviderDefinitionRecord,
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

    Ok(repos
        .users
        .find_by_identifier(instance_id, &identifier)
        .await?
        .map(|user| user.id))
}

async fn create_linked_identity(
    app: &zitadel_app::ApplicationServices,
    _instance_id: &str,
    user_id: &str,
    provider: &ProviderDefinitionRecord,
    identity: &zitadel_oidc::rp::VerifiedExternalIdentity,
) -> anyhow::Result<()> {
    let raw_claims: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&identity.claims)?)?;
    let actor = login_actor_context();
    app.link_identity
        .execute(
            &actor,
            LinkIdentityCommand {
                user_id: user_id.to_string(),
                provider_id: provider.id.clone(),
                external_sub: identity.subject.clone(),
                external_email: Some(identity.email.clone()).filter(|e| !e.is_empty()),
                raw_claims,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("link_identity use case failed: {e}"))?;
    Ok(())
}

async fn resolve_target_schema_id(
    repos: &zitadel_app::repo::Repositories,
    instance_id: &str,
    provider: &ProviderPayload,
) -> anyhow::Result<String> {
    if !provider.target.schema_id.is_empty() {
        return Ok(provider.target.schema_id.clone());
    }
    if provider.target.schema_type.is_empty() {
        return Ok(String::new());
    }

    Ok(repos
        .schemas
        .get_by_type(instance_id, &provider.target.schema_type)
        .await?
        .map(|s| s.id)
        .unwrap_or_default())
}

fn profile_string(profile: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
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
    use zitadel_app::{ApplicationServices, HookPipeline};
    use zitadel_authn::{cookie::CookieConfig, password::Swapper};
    use zitadel_db::{DEFAULT_INSTANCE_ID, DEFAULT_ORG_ID, InstanceContext, with_instance_context};
    use zitadel_fga::FgaService;
    use zitadel_storage::StorageRuntime;

    async fn test_state() -> LoginState {
        let db = zitadel_db::Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        let storage = StorageRuntime::from_config(
            &zitadel_config::Config::default().storage,
            db.clone(),
            zitadel_config::Config::default().session.max_age_secs,
        )
        .await
        .unwrap();
        let fga = Arc::new(FgaService::new(db.clone()));
        let repos = Arc::new(zitadel_server::wiring::build_repositories(
            db.clone(),
            storage.primary.as_ref().replica_db().cloned(),
            storage.transient.clone(),
            fga,
            storage.analytics.clone(),
        ));
        let app = Arc::new(ApplicationServices::new(
            repos,
            Arc::new(HookPipeline::empty()),
            false,
        ));

        LoginState {
            primary: storage.primary.clone(),
            transient: storage.transient.clone(),
            passwords: Arc::new(Swapper::dev()),
            cookie_config: Arc::new(CookieConfig::new(
                vec!["test-secret".into()],
                "localhost",
                false,
            )),
            public_origin: Arc::new("http://localhost:8080".into()),
            public_origin_override: Some(Arc::new("http://localhost:8080".into())),
            conformance_login_html: false,
            rp: Arc::new(DefaultRpService::new(
                zitadel_oidc::rp::ReqwestHttpClient::new(),
                zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
            )),
            pow_secret: "test-pow-secret".into(),
            app,
        }
    }

    fn provider_payload(mode: ProviderLinkingMode) -> ProviderPayload {
        ProviderPayload {
            display_name: "Mock OIDC".into(),
            kind: "custom".into(),
            protocol: "oidc".into(),
            connection: zitadel_app::repo::ProviderConnection {
                issuer: "https://issuer.example".into(),
                client_id: "client-1".into(),
                client_secret: "secret-1".into(),
                scopes: vec!["openid".into(), "profile".into(), "email".into()],
                ..zitadel_app::repo::ProviderConnection::default()
            },
            mapping: zitadel_app::repo::ProviderMapping {
                claims: HashMap::from([
                    ("email".into(), "claims.email".into()),
                    ("display_name".into(), "claims.name".into()),
                ]),
            },
            target: zitadel_app::repo::ProviderTarget {
                schema_type: "human_user".into(),
                schema_id: String::new(),
            },
            linking: zitadel_app::repo::ProviderLinking {
                mode,
                match_by: ProviderMatchBy::VerifiedEmail,
            },
            ..ProviderPayload::default()
        }
    }

    #[tokio::test]
    async fn callback_uri_uses_request_origin_when_unpinned() {
        let mut state = test_state().await;
        state.public_origin_override = None;

        let callback_uri = with_instance_context(
            InstanceContext {
                instance_id: DEFAULT_INSTANCE_ID.to_string(),
                resolved_org_id: None,
                placement_mode: "global".into(),
                region_key: None,
                scheme: "https".into(),
                host: "demo.example.com".into(),
                source: "host".into(),
            },
            async { sso_callback_uri(&state) },
        )
        .await;

        assert_eq!(
            callback_uri,
            "https://demo.example.com/v1/auth/sso/callback"
        );
    }

    #[tokio::test]
    async fn link_only_rejects_unknown_user() {
        let state = test_state().await;
        let provider = ProviderDefinitionRecord {
            id: "provider-1".into(),
            org_id: DEFAULT_ORG_ID.into(),
            created_at: String::new(),
            updated_at: String::new(),
            payload: provider_payload(ProviderLinkingMode::LinkOnly),
        };
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
